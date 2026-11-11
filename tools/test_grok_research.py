"""Budget and failure regressions; all API traffic and credentials are synthetic."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location('grok_runner', Path(__file__).with_name('grok-research.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class Response(io.BytesIO):
    headers = {'Content-Type': 'application/json'}

    def __init__(self, cost=4, text='A substantive synthetic report. ' * 100):
        super().__init__(json.dumps({
            'id': 'synthetic-response', 'status': 'completed', 'model': 'grok-4.7',
            'usage': {'cost_in_usd_ticks': cost * 1e10},
            'output': [{'type': 'message', 'content': [{'type': 'output_text', 'text': text}]}],
        }).encode())


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.local = self.root / 'xai'
        self.local.mkdir()
        self.ledger = self.local / 'usage.json'
        self.ledger.write_text(json.dumps({
            'priorReviewUsd': 0, 'authorizedCeilingUsd': 1500,
            'runScopedAuthorizationActive': 'authorized',
            'runs': {'authorized': {'authorizedCeilingUsd': 500, 'authorization': 'Synthetic test approval'}},
            'requests': [{'name': 'old-unknown', 'reservedUsd': 225, 'status': 'failed-cost-unknown'}],
        }))

    def tearDown(self):
        self.directory.cleanup()

    def invoke(self, names=('test-call',), response=None, run_id='authorized', budget=500, watch=False, extra=(), task_overrides=None):
        tasks = self.root / 'tasks.json'
        tasks.write_text(json.dumps([dict(name=name, instructions='Synthetic fixture only.', **(task_overrides or {}).get(name, {})) for name in names]))
        argv = ['grok-research.py', str(tasks), '--budget-usd', str(budget), '--run-id', run_id, '--parallel', '1']
        argv.extend(extra)
        if watch:
            argv.append('--watch')
        with patch.object(runner, 'LOCAL', self.local), patch.object(runner.sys, 'argv', argv), \
                patch.object(runner.sys, 'platform', 'synthetic'), \
                patch.dict(runner.os.environ, {'XAI_API_KEY': 'synthetic-not-a-real-key'}), \
                patch.object(runner.urllib.request, 'urlopen', side_effect=response or (lambda *a, **k: Response())) as api, \
                contextlib.redirect_stdout(io.StringIO()) as output:
            try:
                runner.main()
                error = None
            except (RuntimeError, SystemExit) as exc:
                error = exc
        self.assertNotIn('synthetic-not-a-real-key', output.getvalue())
        self.assertFalse((self.local / 'review.lock').exists())
        return error, api.call_count, json.loads(self.ledger.read_text())

    def test_multi_gap_spaces_calls_without_blocking_other_models(self):
        calls = []
        model = 'grok-4.20-multi-agent-0309'
        def respond(request, **kwargs):
            calls.append((json.loads(request.data)['model'], time.monotonic()))
            return Response()
        error, count, _ = self.invoke(
            names=('multi-a', 'multi-b', 'ordinary'), response=respond,
            extra=('--parallel', '2', '--multi-parallel', '1', '--multi-gap-seconds', '1'),
            task_overrides={'multi-a': {'model': model}, 'multi-b': {'model': model}})
        self.assertIsNone(error)
        self.assertEqual(count, 3)
        multi = sorted(stamp for kind, stamp in calls if kind == model)
        ordinary = [stamp for kind, stamp in calls if kind != model]
        self.assertGreaterEqual(multi[1] - multi[0], 1)
        self.assertLess(ordinary[0], multi[1])

    def test_new_id_cannot_reset_authorization(self):
        error, calls, ledger = self.invoke(run_id='fresh-budget')
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(calls, 0)
        self.assertEqual(ledger['runScopedAuthorizationActive'], 'authorized')

    def test_registered_budget_is_immutable_and_history_is_separate(self):
        error, calls, _ = self.invoke(budget=501)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(calls, 0)
        error, calls, ledger = self.invoke()
        self.assertIsNone(error)
        self.assertEqual(calls, 1)
        self.assertEqual(runner.exposure(ledger, 'authorized'), 4)
        self.assertEqual(runner.exposure(ledger), 229)
        self.assertEqual(ledger['runs']['authorized']['authorization'], 'Synthetic test approval')

    def test_known_charge_survives_report_write_failure(self):
        original = Path.write_text

        def fail_artifact(path, *args, **kwargs):
            if path.name.endswith('.response.json'):
                raise OSError('Synthetic disk error')
            return original(path, *args, **kwargs)

        with patch.object(Path, 'write_text', fail_artifact):
            error, calls, ledger = self.invoke(response=lambda *a, **k: Response(cost=40))
        self.assertIsInstance(error, SystemExit)
        self.assertEqual(calls, 1)
        self.assertEqual(ledger['requests'][-1]['status'], 'failed-local-processing')
        self.assertEqual(runner.exposure(ledger, 'authorized'), 40)

    def test_failure_before_send_is_zero(self):
        original = Path.write_text

        def fail_request(path, *args, **kwargs):
            if path.name.endswith('.request.json'):
                raise OSError('Synthetic disk error')
            return original(path, *args, **kwargs)

        with patch.object(Path, 'write_text', fail_request):
            error, calls, ledger = self.invoke()
        self.assertIsInstance(error, SystemExit)
        self.assertEqual(calls, 0)
        self.assertEqual(ledger['requests'][-1]['status'], 'failed-before-send')
        self.assertEqual(runner.exposure(ledger, 'authorized'), 0)

    def test_401_stops_queued_calls_and_preserves_sent_hold(self):
        def denied(*args, **kwargs):
            raise urllib.error.HTTPError('https://api.x.ai/v1/responses', 401, 'Synthetic denial', {}, None)
        error, calls, ledger = self.invoke(names=('sent', 'queued'), response=denied)
        self.assertIsInstance(error, SystemExit)
        self.assertEqual(calls, 1)
        self.assertEqual(ledger['requests'][-1]['name'], 'sent')
        self.assertEqual(ledger['requests'][-1]['status'], 'failed-cost-unknown')
        self.assertFalse(any(r['name'] == 'queued' for r in ledger['requests']))
        self.assertEqual(runner.exposure(ledger, 'authorized'), 25)

    def test_invalid_report_is_still_charged(self):
        error, calls, ledger = self.invoke(response=lambda *a, **k: Response(cost=3, text='Acknowledged.'))
        self.assertIsInstance(error, SystemExit)
        self.assertEqual(calls, 1)
        self.assertEqual(ledger['requests'][-1]['validation'], 'insufficient-report')
        self.assertEqual(runner.exposure(ledger, 'authorized'), 3)

    def test_updated_cost_stops_queued_dispatch(self):
        error, calls, ledger = self.invoke(names=('expensive', 'queued'), response=lambda *a, **k: Response(cost=490))
        self.assertIsInstance(error, SystemExit)
        self.assertEqual(calls, 1)
        self.assertEqual(ledger['requests'][-1]['name'], 'expensive')
        self.assertFalse(any(r['name'] == 'queued' for r in ledger['requests']))
        self.assertEqual(runner.exposure(ledger, 'authorized'), 490)

    def test_stream_error_diagnostics_do_not_capture_arbitrary_message(self):
        error = runner.ProviderStreamError({'error': {'code': 'RESOURCE_EXHAUSTED', 'message': 'Token limit secret-like-payload'}})
        self.assertEqual(error.provider_code, 'RESOURCE_EXHAUSTED')
        self.assertEqual(error.category, 'token-limit')
        self.assertNotIn('secret-like-payload', str(error))
        self.assertEqual(runner.ProviderStreamError({'code': 'unsafe value'}).provider_code, 'unclassified')

    def test_unfundable_queue_is_preserved_without_dispatch_or_reservation(self):
        ledger = json.loads(self.ledger.read_text())
        ledger['requests'].append({'name': 'already', 'runId': 'authorized', 'status': 'completed', 'costUsd': 480})
        self.ledger.write_text(json.dumps(ledger))
        error, calls, ledger = self.invoke()
        self.assertIsInstance(error, SystemExit)
        self.assertEqual(calls, 0)
        self.assertFalse(any(r['name'] == 'test-call' for r in ledger['requests']))

    def test_429_cooldown_keeps_other_model_working(self):
        starts = []
        def response(request, **kwargs):
            model = json.loads(request.data)['model']
            starts.append((model, time.monotonic()))
            if len(starts) == 1:
                raise urllib.error.HTTPError('https://api.x.ai/v1/responses',429,'limit',{'Retry-After':'1'},io.BytesIO(b'{"error":{"code":"rate_limit","message":"rate limit"}}'))
            return Response(cost=1)
        names = ('limited','replacement','other-model')
        overrides = {'limited': {'model':'grok-4.20-multi-agent-0309'}, 'replacement': {'model':'grok-4.20-multi-agent-0309'}}
        error,calls,ledger = self.invoke(names=names,response=response,extra=('--parallel','1'),task_overrides=overrides)
        self.assertIsInstance(error,SystemExit)
        self.assertEqual(calls,3)
        self.assertEqual(starts[1][0],'grok-4.7')
        self.assertGreaterEqual(starts[2][1]-starts[0][1],1)
        self.assertEqual(runner.exposure(ledger,'authorized'),27)

    def test_retry_after_date_and_invalid_values(self):
        self.assertEqual(runner.retry_delay('120'), 120)
        self.assertEqual(runner.retry_delay('Wed, 04 Nov 2026 23:00:00 GMT', now=1793833020), 180)
        self.assertEqual(runner.retry_delay('invalid'), 60)
        self.assertEqual(runner.retry_delay('-1'), 60)

    def test_resume_does_not_retry_unknown_or_completed_tasks(self):
        self.invoke(names=('recorded',))
        error, calls, ledger = self.invoke(names=('recorded', 'new'), extra=('--resume',))
        self.assertIsNone(error)
        self.assertEqual(calls, 1)
        self.assertEqual(sum(r['name'] == 'recorded' for r in ledger['requests']), 1)
        ledger['requests'][-1].pop('costUsd')
        ledger['requests'][-1]['status'] = 'failed-cost-unknown'
        self.ledger.write_text(json.dumps(ledger))
        error, calls, _ = self.invoke(names=('new',), extra=('--resume',))
        self.assertEqual(calls, 0)

    def test_mixed_model_fleet_respects_limits_and_dispatch_budget(self):
        lock = threading.Lock()
        active, peak = {}, {}
        def response(request, **kwargs):
            model = json.loads(request.data)['model']
            with lock:
                active[model] = active.get(model, 0) + 1
                peak[model] = max(peak.get(model, 0), active[model])
                snapshot = json.loads(self.ledger.read_text())
                self.assertLessEqual(runner.exposure(snapshot, 'authorized'), 500)
                self.assertLessEqual(sum(r['status'] in {'reserved', 'pending'} for r in snapshot['requests']), 4)
            time.sleep(.025)
            with lock:
                active[model] -= 1
            return Response(cost=1)
        names = ('m1','m2','m3','f1','f2','f3','f4')
        overrides = {n: {'model': 'grok-4.20-multi-agent-0309' if n.startswith('m') else 'grok-4.7'} for n in names}
        error, calls, ledger = self.invoke(names=names, response=response, extra=('--parallel','4','--multi-parallel','1'), task_overrides=overrides)
        self.assertIsNone(error)
        self.assertEqual(calls, 7)
        self.assertEqual(peak['grok-4.20-multi-agent-0309'], 1)
        self.assertGreater(peak['grok-4.7'], 1)
        self.assertEqual(runner.exposure(ledger, 'authorized'), 7)

    def test_error_body_timeout_keeps_cost_hold_and_401_stop(self):
        class BrokenBody:
            def read(self, count): raise TimeoutError('Synthetic stalled error body')
            def close(self): pass
        def response(*args, **kwargs):
            raise urllib.error.HTTPError('https://api.x.ai/v1/responses',401,'denied',{},BrokenBody())
        error, calls, ledger = self.invoke(names=('first','second'),response=response)
        self.assertIsInstance(error,SystemExit)
        self.assertEqual(calls,1)
        self.assertEqual(runner.exposure(ledger,'authorized'),25)

    def test_exact_schema_and_money_validation(self):
        with self.assertRaises(ValueError):
            runner.final_report('{"summary":"ok","extra":"no"}', {'summary': 2})
        report, interim = runner.final_report('{"summary":"hi"}\n{"summary":"ok"}', {'summary': 2})
        self.assertEqual(report, {'summary': 'ok'})
        self.assertEqual(interim, 1)
        for value in [True, -1, float('nan'), float('inf')]:
            with self.assertRaises(ValueError):
                runner.money(value)
        self.assertEqual(runner.exposure({'requests': [{'costUsd': 40, 'reservedUsd': 25, 'status': 'failed-cost-unknown'}]}), 40)

    def test_final_receipt_does_not_wait_for_stream_close(self):
        class Stream:
            headers = {'Content-Type': 'text/event-stream'}

            def __iter__(self):
                yield b'data: {"type":"response.created","response":{"id":"synthetic"}}\n'
                yield b'\n'
                yield b'data: {"type":"response.completed","response":{"id":"synthetic","status":"completed","usage":{"cost_in_usd_ticks":40000000000}}}\n'
                yield b'\n'
                raise AssertionError('Do not read beyond the final provider receipt')
        created = []
        result = runner.read_response(Stream(), created.append)
        self.assertEqual(created, ['synthetic'])
        self.assertEqual(result['usage']['cost_in_usd_ticks'], 40000000000)

    def test_code_proposals_require_real_edits_and_safe_paths(self):
        proposal = {'summary': 'Synthetic', 'files': [{'path': 'api/src/example.rs', 'content': '// synthetic'}],
                    'edits': [], 'tests': [], 'concerns': []}
        self.assertEqual(runner.final_proposal(json.dumps(proposal))[0], proposal)
        proposal['files'][0]['path'] = '../outside.rs'
        with self.assertRaises(ValueError):
            runner.final_proposal(json.dumps(proposal))
        proposal['files'] = []
        with self.assertRaises(ValueError):
            runner.final_proposal(json.dumps(proposal))

    def test_continuous_queue_dispatches_new_work_without_a_new_runner(self):
        calls = []

        def complete_and_replenish(*args, **kwargs):
            calls.append(True)
            if len(calls) == 1:
                path = self.root / 'tasks.json'
                tasks = json.loads(path.read_text())
                tasks.append({'name': 'follow-up', 'instructions': 'Synthetic follow-up.'})
                temp = path.with_suffix('.tmp')
                temp.write_text(json.dumps(tasks))
                temp.replace(path)
            return Response()

        error, count, ledger = self.invoke(response=complete_and_replenish, watch=True)
        self.assertIsNone(error)
        self.assertEqual(count, 2)
        self.assertEqual([r['name'] for r in ledger['requests'][-2:]], ['test-call', 'follow-up'])
        self.assertEqual(runner.exposure(ledger, 'authorized'), 8)


if __name__ == '__main__':
    unittest.main()
