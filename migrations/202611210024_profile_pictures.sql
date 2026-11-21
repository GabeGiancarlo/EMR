-- Profile pictures for clients and staff.
--
-- A row is one person's chosen picture: either an uploaded photo or one of the ten Nexus
-- motifs picked as a default. No row means the automatic default, which the app derives
-- from the person's id. The picture lives in its own table on purpose: changing it must not
-- touch the client or staff row, whose updates invalidate the knowledge index, move the
-- record's version and belong to registration.
--
-- Photos are small by rule (512 KiB, checked again in the API with the image's own header)
-- and only JPEG, PNG or WebP. SVG is not accepted: it can carry script.
CREATE TABLE profile_pictures (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 client_id uuid,
 staff_id uuid,
 motif text CHECK(motif IN ('sprig','fern','sprout','pine','hill','river','cairn','slab','sun','moon')),
 content_type text CHECK(content_type IN ('image/jpeg','image/png','image/webp')),
 bytes bytea CHECK(octet_length(bytes) BETWEEN 1 AND 524288),
 sha256 text CHECK(sha256 ~ '^[0-9a-f]{64}$'),
 updated_by uuid NOT NULL,
 updated_at timestamptz NOT NULL DEFAULT now(),
 -- One person per row, and the row is either a chosen default or a photo with its facts.
 CHECK(num_nonnulls(client_id, staff_id) = 1),
 CHECK(num_nonnulls(motif, bytes) = 1),
 CHECK(num_nonnulls(bytes, content_type, sha256) IN (0, 3)),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id),
 FOREIGN KEY(staff_id,organization_id) REFERENCES staff(id,organization_id),
 FOREIGN KEY(updated_by,organization_id) REFERENCES staff(id,organization_id)
);
CREATE UNIQUE INDEX profile_pictures_client ON profile_pictures(client_id) WHERE client_id IS NOT NULL;
CREATE UNIQUE INDEX profile_pictures_staff ON profile_pictures(staff_id) WHERE staff_id IS NOT NULL;
