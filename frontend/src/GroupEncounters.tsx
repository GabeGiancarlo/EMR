import { useState } from "react";
import {
  clientName,
  initials,
  label,
  numeric,
  value,
  withOriginalUpdatedAt,
} from "./api";
import type { WorkflowProps } from "./CarePlanning";
import type { Row } from "./api";
import { facilityYmd, formatFacilityTime, formatYmdLong, safeTimeZone } from "./calendar";
import { Badge, Button, Empty, Icon, Panel } from "./ui";

export default function GroupEncounters({
  data,
  canWrite,
  create,
  edit,
  action,
  mutate,
  initialSelectedId = "",
  onTelehealth,
}: WorkflowProps & { initialSelectedId?: string; onTelehealth?:(row:Row)=>void }) {
  const [selectedId, setSelectedId] = useState(initialSelectedId);
  const [error, setError] = useState("");
  const [savingAttendance, setSavingAttendance] = useState<Record<string, boolean>>({});
  const [pendingStatus, setPendingStatus] = useState<Record<string, string>>(
    {},
  );
  const groups = [...data.groups].sort((a, b) =>
    value(b, "starts_at").localeCompare(value(a, "starts_at")),
  );
  const selected = groups.find((g) => g.id === selectedId) || groups[0];
  const groupZone = (group: Row) => safeTimeZone(value(data.facilities.find(facility => facility.id === group.facility_id), "timezone"));
  const groupDate = (group: Row, field = "starts_at") => {
    const zone = groupZone(group);
    const instant = new Date(value(group,field));
    return `${formatYmdLong(facilityYmd(instant,zone))} · ${formatFacilityTime(instant,zone)} ${zone}`;
  };
  const attendance = selected
    ? data.group_attendance.filter((a) => a.group_id === selected.id)
    : [];
  const staffName = (id: unknown) =>
    value(
      data.staff.find((s) => s.id === id),
      "name",
    );
  async function setStatus(id: string, status: string) {
    const row = data.group_attendance.find((item) => item.id === id);
    setPendingStatus((current) => ({ ...current, [id]: status }));
    setSavingAttendance((current) => ({ ...current, [id]: true }));
    setError("");
    try {
      await mutate(
        `/api/group_attendance/${id}`,
        withOriginalUpdatedAt(row, { status }),
        "PATCH",
      );
      setPendingStatus((current) => {
        const next = { ...current };
        delete next[id];
        return next;
      });
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Attendance could not be updated.",
      );
    } finally {
      setSavingAttendance((current) => ({ ...current, [id]: false }));
    }
  }
  if (!groups.length)
    return (
      <Panel>
        <Empty
          icon="team"
          title="One shared experience. Individual care records."
          description="Create a group encounter, add participants, and capture attendance. Each participant has their own linked session for notes and billing."
          action={
            canWrite ? (
              <Button variant="secondary" onClick={() => create("groups")}>
                Schedule a group
              </Button>
            ) : undefined
          }
        />
      </Panel>
    );
  return (
    <div className="care-planning groups-workspace">
      <aside className="plan-list">
        <div className="plan-list-head">
          <h3>Group encounters</h3>
          {canWrite && (
            <button
              className="icon-button"
              aria-label="Schedule group"
              onClick={() => create("groups")}
            >
              <Icon name="plus" size={17} />
            </button>
          )}
        </div>
        {groups.map((g) => (
          <button
            key={g.id}
            className={`plan-list-item ${g.id === selected?.id ? "selected" : ""}`}
            onClick={() => setSelectedId(g.id)}
          >
            <strong>{value(g, "title")}</strong>
            <Badge status={g.status} />
            <small>{groupDate(g)}</small>
          </button>
        ))}
      </aside>
      {selected && (
        <div className="plan-detail">
          <Panel
            title={value(selected, "title")}
            subtitle={`${label(value(selected, "kind"))} · ${groupDate(selected)}`}
            action={<Badge status={selected.status} />}
          >
            <div className="group-overview">
              <div>
                <span>Facilitator</span>
                <strong>{staffName(selected.clinician_id)}</strong>
              </div>
              <div>
                <span>Location</span>
                <strong>{value(selected, "location")}</strong>
              </div>
              <div>
                <span>Participants</span>
                <strong>
                  {attendance.length} / {numeric(selected, "capacity")}
                </strong>
              </div>
              <div>
                <span>End time</span>
                <strong>
                  {groupDate(selected, "ends_at")}
                </strong>
              </div>
            </div>
            {canWrite && selected.status === "scheduled" && (
              <div className="plan-actions">
                {onTelehealth&&["admin","clinical_lead","clinician"].includes(value(data.actor,"role"))&&(value(data.actor,"role")!=="clinician"||selected.clinician_id===data.actor?.id)&&<Button variant="secondary small" onClick={()=>onTelehealth(selected)}>Open telehealth</Button>}
                <Button
                  variant="secondary small"
                  onClick={() => edit("groups", selected)}
                >
                  Edit encounter
                </Button>
                <Button
                  variant="secondary small"
                  onClick={() =>
                    action(
                      `/api/groups/${selected.id}/cancel`,
                      "Cancel group encounter",
                      "Cancel the scheduled group and any linked scheduled participant sessions.",
                      [
                        {
                          key: "reason",
                          label: "Cancellation reason",
                          type: "textarea",
                          required: true,
                        },
                      ],
                      {},
                      "Cancel group",
                    )
                  }
                >
                  Cancel group
                </Button>
                <Button
                  variant="small"
                  onClick={() =>
                    action(
                      `/api/groups/${selected.id}/complete`,
                      "Complete group encounter",
                      "Confirm the group has ended and every participant has a final attendance status. Individual documentation is completed in each participant’s chart.",
                      [],
                      {},
                      "Complete group",
                    )
                  }
                >
                  Complete encounter <Icon name="check" size={16} />
                </Button>
              </div>
            )}
          </Panel>
          <Panel
            title="Participant roster"
            subtitle="Attendance belongs to each client. Notes and billing stay separate."
            action={
              canWrite && selected.status === "scheduled" ? (
                <Button
                  variant="secondary small"
                  onClick={() =>
                    create("group_attendance", { group_id: selected.id })
                  }
                  disabled={attendance.length >= numeric(selected, "capacity")}
                >
                  <Icon name="plus" size={16} /> Add participant
                </Button>
              ) : undefined
            }
          >
            {error && (
              <div className="error-banner group-error" role="alert">
                {error}
              </div>
            )}
            {attendance.length ? (
              <div className="participant-list">
                {attendance.map((a) => {
                  const client = data.clients.find((c) => c.id === a.client_id);
                  return (
                    <article key={a.id}>
                      <span className="avatar">
                        {initials(clientName(client))}
                      </span>
                      <div className="grow">
                        <strong>{clientName(client)}</strong>
                        <small>
                          Individual session{" "}
                          {value(a, "session_id").slice(0, 8)}
                        </small>
                      </div>
                      {canWrite && selected.status === "scheduled" ? (
                        <select
                          aria-label={`Attendance for ${clientName(client)}`}
                          className="compact-select"
                          value={pendingStatus[a.id] ?? value(a, "status")}
                          disabled={savingAttendance[a.id] === true}
                          onChange={(e) => void setStatus(a.id, e.target.value)}
                        >
                          {["scheduled", "attended", "no_show", "excused"].map(
                            (s) => (
                              <option key={s} value={s}>
                                {label(s)}
                              </option>
                            ),
                          )}
                        </select>
                      ) : (
                        <Badge status={a.status} />
                      )}
                      <Button
                        variant="text small"
                        disabled={
                          a.status !== "attended" ||
                          !data.permissions?.notes?.write
                        }
                        onClick={() =>
                          create("notes", {
                            client_id: a.client_id,
                            session_id: a.session_id,
                          })
                        }
                      >
                        Document care <Icon name="notes" size={15} />
                      </Button>
                    </article>
                  );
                })}
              </div>
            ) : (
              <Empty
                icon="clients"
                title="Bring the group together"
                description="Add clients with active care episodes. Attendance creates a linked individual session for each participant."
              />
            )}
          </Panel>
          <div className="info-strip">
            <Icon name="shield" size={19} />
            <p>
              Keep shared group context separate from personal disclosures.
              Clinical notes belong to individual clients and follow their
              record-access rules.
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
