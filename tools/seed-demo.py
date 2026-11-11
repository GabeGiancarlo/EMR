#!/usr/bin/env python3
"""Fill an empty Nexus database with synthetic demonstration records.

Every record created here is invented. Nothing is copied from another database, and
the script performs no reset or delete. Successful requests are journaled so an
interrupted run can resume.

It talks to a Nexus API running in LOCAL mode on this machine (NEXUS_LOCAL_DEV=1,
loopback only), because seeding acts as the workspace administrator before anyone can
sign in. To seed a hosted database, start a local-mode API on your own machine with
DATABASE_URL pointing at that database, run this script against it, then stop it.
See docs/deployment/demo-hosting.md.

The staff emails you pass are the accounts that will be able to sign in to the hosted
demo: each must match the verified email of an identity-provider account.
"""
import argparse
import datetime as dt
import json
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request
from zoneinfo import ZoneInfo

ROLES = [('clinical_lead', 'Clinical Lead'), ('clinician', 'Clinician'),
         ('billing', 'Billing'), ('executive', 'Executive'),
         ('support', 'Intake Support')]
BASE = ''
JOURNAL = Path()
state = {}
actor = None


def save():
    temporary = JOURNAL.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(JOURNAL)


def request(path, body=None, method=None):
    headers = {'Content-Type': 'application/json'}
    if actor:
        headers['x-nexus-actor'] = actor
    raw = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=raw, headers=headers,
                                 method=method or ('GET' if body is None else 'POST'))
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            data = response.read()
            return json.loads(data) if data else None
    except urllib.error.HTTPError as error:
        raise RuntimeError(f'{req.method} {path}: HTTP {error.code}: '
                           f'{error.read().decode()}') from error


def step(name, path, body, method='POST'):
    if name not in state:
        state[name] = request(path, body, method)
        save()
        print('Created:', name, flush=True)
    return state[name]


def create(step_name, resource, **fields):
    return step(step_name, '/api/' + resource, fields)


def transition(name, case, to):
    return step(name, f'/api/intake-cases/{case["id"]}/transition', {
        '_expected_updated_at': case['updated_at'], 'to_status': to,
        'reason': 'Synthetic walkthrough: reviewed example intake decision.',
        'attested': True,
    })


def signed_form(name, client, episode_id, template, answers):
    form = create(name, 'form_responses', client_id=client['id'],
                  episode_id=episode_id, template_id=template['id'],
                  answers=answers, source_type='manual', source_excerpt='',
                  field_provenance={})
    step(name + '_review', f'/api/form_responses/{form["id"]}/review', {})
    step(name + '_sign', f'/api/form_responses/{form["id"]}/sign', {})
    return form


def main():
    global actor, BASE, JOURNAL, state
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--api', required=True,
                        help='Local-mode API on this machine, for example http://127.0.0.1:8095')
    parser.add_argument('--admin-email', required=True,
                        help='Email of the person who will sign in as the workspace administrator.')
    parser.add_argument('--staff', action='append', default=[], metavar='ROLE=EMAIL',
                        help='Sign-in email for another role (' + ', '.join(r for r, _ in ROLES) +
                             '). Repeatable. Roles left out get a placeholder that cannot sign in.')
    parser.add_argument('--organization', default='Nexus Demo — SYNTHETIC DATA')
    parser.add_argument('--journal', default=str(Path(__file__).resolve().parent.parent / '.local' / 'demo-seed-state.json'),
                        help='Where progress is recorded so a run can resume (default: .local/demo-seed-state.json).')
    parser.add_argument('--confirm-synthetic-demo', action='store_true',
                        help='Confirm the target database is an empty demonstration database.')
    args = parser.parse_args()
    if not args.confirm_synthetic_demo:
        parser.error('Pass --confirm-synthetic-demo to confirm the target is an empty demonstration database.')
    target = urllib.parse.urlsplit(args.api)
    if target.scheme != 'http' or target.hostname not in ('127.0.0.1', 'localhost', '::1'):
        parser.error('--api must be a local-mode API on this machine (http://127.0.0.1:PORT).')
    emails = {}
    for item in args.staff:
        role, _, email = item.partition('=')
        if role not in dict(ROLES) or '@' not in email:
            parser.error(f'--staff expects ROLE=EMAIL with a known role, got: {item}')
        emails[role] = email.strip().lower()
    admin_email = args.admin_email.strip().lower()
    used = [admin_email] + list(emails.values())
    if len(set(used)) != len(used):
        parser.error('Each sign-in email can belong to only one staff member.')
    BASE = args.api.rstrip('/')
    ORG = args.organization
    JOURNAL = Path(args.journal)
    JOURNAL.parent.mkdir(parents=True, exist_ok=True)
    state = json.loads(JOURNAL.read_text()) if JOURNAL.exists() else {}
    actor = state.get('setup', {}).get('actor', {}).get('id')
    health = request('/healthz')
    if health.get('authentication') != 'deferred-local-only':
        raise SystemExit('This API is not in local mode. Seeding needs a local-mode API on this '
                         'machine pointed at the demonstration database.')
    actor_before = actor
    actor = None  # The unauthenticated bootstrap lists organizations without acting as anyone.
    bootstrap = request('/api/bootstrap')
    actor = actor_before
    organizations = bootstrap.get('organizations', [])
    if organizations and (len(organizations) != 1 or organizations[0]['name'] != ORG):
        raise SystemExit('Refusing to seed: this database already holds a different workspace.')
    if organizations and 'setup' not in state:
        raise SystemExit('The demonstration workspace exists but its journal is missing; inspect manually.')
    setup = step('setup', '/api/setup', {
        'organization_name': ORG, 'facility_name': 'Synthetic Recovery Center',
        'admin_name': 'Demo Admin (Synthetic)',
        'admin_email': admin_email,
    })
    actor = setup['actor']['id']
    facility = setup['facility']
    step('facility_timezone', f'/api/facilities/{facility["id"]}', {
        '_expected_updated_at': facility['updated_at'], 'timezone': 'America/New_York',
    }, 'PATCH')
    zone = ZoneInfo('America/New_York')
    state.setdefault('run_date', dt.datetime.now(zone).date().isoformat())
    save()
    today = dt.date.fromisoformat(state['run_date'])
    date = lambda offset: (today + dt.timedelta(days=offset)).isoformat()
    stamp = lambda offset, hour: dt.datetime.combine(
        today + dt.timedelta(days=offset), dt.time(hour), zone).isoformat()

    staff = {}
    for role, label in ROLES:
        staff[role] = create('staff_' + role, 'staff', name=f'Demo {label} (Synthetic)',
                             email=emails.get(role, f'demo-{role}@example.invalid'), role=role,
                             facility_id=facility['id'], active=True)
    clinician = staff['clinician']
    intake_template = create('intake_template', 'templates',
        name='Synthetic intake review', kind='intake',
        description='Demonstration-only form; not a clinical instrument or legal consent.',
        fields=[{'key': 'review', 'label': 'Reviewed intake summary', 'type': 'textarea',
                 'required': True}], active=True, applies_to_stage='intake',
        applies_to_care_level='outpatient', required_for_stage=True)
    progress_template = create('progress_template', 'templates',
        name='Synthetic progress review', kind='progress',
        description='Example structure for exploring the custom form editor.',
        fields=[{'key': 'subjective', 'label': 'Client report', 'type': 'textarea', 'required': True},
                {'key': 'plan', 'label': 'Next step', 'type': 'textarea', 'required': True}],
        active=True, applies_to_stage='active', applies_to_care_level='outpatient',
        required_for_stage=False)
    program = create('program', 'care-programs', facility_id=facility['id'],
        code='DEMO-OP', name='Synthetic Adult Outpatient', level_of_care='outpatient',
        min_age=18, max_age=None, capacity=12, active=True,
        required_intake_template_ids=[intake_template['id']],
        required_discharge_template_ids=[], change_reason='Initial synthetic demonstration configuration.')
    services = {}
    for kind, code, label, charge in [('individual', 'DEMO-IND', 'Synthetic individual visit', 15000),
                                     ('group', 'DEMO-GRP', 'Synthetic skills group', 7500)]:
        services[kind] = create('service_' + kind, 'program-services',
            program_id=program['id'], code_system='internal', service_code=code,
            name=label, session_kind=kind, min_minutes=30, default_minutes=60,
            max_minutes=90, default_charge_cents=charge, note_template_id=progress_template['id'],
            active=True, change_reason='Synthetic service for local workflow demonstration.')

    clients, cases = [], []
    for index, first in enumerate(['Alex', 'Jordan', 'Taylor']):
        client = create('client_' + first, 'clients', first_name=first,
            last_name='Synthetic', birth_date=f'199{index}-06-15',
            email=f'{first.lower()}.synthetic@example.invalid', phone='',
            status='inquiry', facility_id=facility['id'], primary_clinician_id=clinician['id'],
            preferred_language='English', pronouns='they/them',
            emergency_contact='Synthetic demonstration record; no real contact.')
        clients.append(client)
        case = step('intake_' + first, f'/api/clients/{client["id"]}/intake', {
            'program_id': program['id'], 'starts_on': date(-2), 'received_on': date(-2),
            'owner_id': staff['support']['id'], 'referral_type': 'self',
            'referrer_name': 'Synthetic self referral', 'referrer_contact': '',
            'referral_reason': 'Synthetic demonstration only. Explore connected intake and care workflows.',
        })
        if index < 2:
            step('identity_' + first, f'/api/clients/{client["id"]}/identity-checks', {
                'method': 'patient_report', 'outcome': 'confirmed',
                'reason': 'Synthetic identity attestation for demonstration only.', 'attested': True,
            })
            for kind in ['treatment', 'recording', 'processing']:
                create('consent_' + first + '_' + kind, 'consents', client_id=client['id'],
                    kind=kind, granted_on=date(-2),
                    scope='Synthetic demonstration attestation only; no real participant or recording.')
            signed_form('intake_form_' + first, client, case['episode_id'], intake_template,
                        {'review': 'Synthetic intake reviewed for the local demonstration; no real clinical facts.'})
        case = transition('screening_' + first, case, 'screening')
        if index < 2:
            case = transition('ready_' + first, case, 'ready')
            case = transition('admitted_' + first, case, 'admitted')
        cases.append(case)

    past_sessions = []
    for i, client in enumerate(clients[:2]):
        session = create(f'past_session_{i}', 'sessions', client_id=client['id'],
            episode_id=cases[i]['episode_id'], clinician_id=clinician['id'],
            starts_at=stamp(-1, 9 + i), ends_at=stamp(-1, 10 + i), kind='individual',
            status='completed', location='Synthetic office visit',
            program_service_id=services['individual']['id'])
        past_sessions.append(session)
        note = create(f'note_{i}', 'notes', client_id=client['id'], session_id=session['id'],
            template_id=progress_template['id'], title='Synthetic progress note — ' + client['first_name'],
            body='SYNTHETIC WALKTHROUGH DATA. This example note demonstrates documentation status, '
                 'review, signatures, and billing linkage. It contains no real clinical assessment.')
        if i == 0:
            step('signed_note', f'/api/notes/{note["id"]}/sign', {})

    create('upcoming_session', 'sessions', client_id=clients[0]['id'],
        episode_id=cases[0]['episode_id'], clinician_id=clinician['id'],
        starts_at=stamp(0, 15), ends_at=stamp(0, 16), kind='individual', status='scheduled',
        location='Synthetic telehealth visit', program_service_id=services['individual']['id'])
    group = create('group', 'groups', facility_id=facility['id'], clinician_id=clinician['id'],
        title='Synthetic recovery skills circle', kind='skills', starts_at=stamp(1, 10),
        ends_at=stamp(1, 11), capacity=8, location='Synthetic group room',
        program_service_id=services['group']['id'])
    for i, client in enumerate(clients[:2]):
        create(f'group_attendance_{i}', 'group_attendance', group_id=group['id'],
            client_id=client['id'], episode_id=cases[i]['episode_id'], status='scheduled')

    plan = create('plan', 'treatment_plans', client_id=clients[0]['id'],
        episode_id=cases[0]['episode_id'], title='Synthetic care plan',
        presenting_needs='Synthetic example for demonstrating goal tracking, not a treatment recommendation.',
        strengths='Synthetic example: participant identifies a support person.', next_review_date=date(7))
    goal = create('goal', 'goals', plan_id=plan['id'],
        description='Synthetic participation goal', measure='Self-reported practice days per week',
        baseline='Example baseline: 1 day', target='Example target: 3 days', target_date=date(30))
    step('plan_active', f'/api/treatment_plans/{plan["id"]}/activate', {})
    create('plan_review', 'plan_reviews', plan_id=plan['id'],
        summary='Synthetic review demonstrating an append-only goal snapshot.',
        goal_progress=[{'goal_id': goal['id'], 'status': 'on_track',
                        'observation': 'Synthetic example: two practice days reported.'}],
        next_review_date=date(7))
    for i, client in enumerate(clients[:2]):
        policy = create(f'policy_{i}', 'policies', client_id=client['id'],
            payer_name='Synthetic Demonstration Payer', member_id=f'DEMO-MEMBER-{i+1}',
            group_number='SYNTHETIC', effective_date=date(-30), status='active')
        auth = create(f'authorization_{i}', 'authorizations', client_id=client['id'],
            policy_id=policy['id'], authorization_number=f'DEMO-AUTH-{i+1}',
            service_code='DEMO-IND', units_approved=12, starts_on=date(-30), ends_on=date(90), status='active')
        claim = create(f'claim_{i}', 'claims', client_id=client['id'], policy_id=policy['id'],
            session_id=past_sessions[i]['id'], authorization_id=auth['id'], service_code='DEMO-IND',
            diagnosis_code='SYNTHETIC-ONLY', amount_cents=15000, units=1)
        if i == 0:
            step('claim_ready', f'/api/claims/{claim["id"]}/ready', {})
            step('claim_submitted', f'/api/claims/{claim["id"]}/record-submission',
                 {'external_reference': 'SYNTHETIC-NO-REAL-SUBMISSION'})
            create('payment', 'payments', claim_id=claim['id'], amount_cents=10000,
                received_on=date(0), reference='SYNTHETIC-NO-REAL-PAYMENT', source='payer')
    for i, (title, role, patient) in enumerate([
        ('Review synthetic unsigned progress note', 'clinician', clients[1]),
        ('Complete synthetic intake checklist', 'support', clients[2]),
        ('Inspect synthetic claim readiness blockers', 'billing', clients[1]),
    ]):
        create(f'task_{i}', 'tasks', client_id=patient['id'], assignee_id=staff[role]['id'],
            title=title, due_date=date(0), priority='high' if i == 0 else 'normal', status='open',
            description='Demonstration task only. No real patient, payer, or clinical work is represented.')
    create('recording_policy', 'recording-policies', title='Synthetic local recording policy',
        purpose='Demonstration of explicit participant agreement. No blanket consent is implied.',
        retention_days=7, allow_video=True)
    result = request('/api/bootstrap')
    state['summary'] = {'organization': ORG, 'api': BASE,
                        'admin_id': actor, 'run_date': date(0),
                        'metrics': result['metrics']}
    save()
    print(json.dumps(state['summary'], indent=2))


if __name__ == '__main__':
    main()
