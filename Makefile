# Fallback for contributors who do not use `just`.

EMR_PORT ?= 8090

.PHONY: dev api dev-stop open-health api-test who-emr-port

dev: api

api:
	@NEXUS_LOCAL_DEV=1 HOST=127.0.0.1 PORT=$(EMR_PORT) cargo run -p emr-api

dev-stop:
	@echo "Press Ctrl+C in the terminal that started this API. Other checkouts are not stopped."

open-health:
	@command -v open >/dev/null && open "http://127.0.0.1:$(EMR_PORT)/healthz" || echo "Open http://127.0.0.1:$(EMR_PORT)/healthz"

who-emr-port:
	@lsof -iTCP:$(EMR_PORT) -sTCP:LISTEN || echo "(nothing listening on $(EMR_PORT))"

api-test:
	@echo "Health:"
	@curl -s "http://127.0.0.1:$(EMR_PORT)/healthz" | jq '.' || echo "API not running"
	@echo ""
	@echo "Workspace setup:"
	@curl -fsS "http://127.0.0.1:$(EMR_PORT)/api/bootstrap" | jq '.' || echo "API not running"
