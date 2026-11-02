ALTER TABLE claims ADD COLUMN units integer NOT NULL DEFAULT 1 CHECK(units > 0);
