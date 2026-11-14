-- A UUID holds any 128 bits: a default is no guarantee, since an insert may write any other.
create table made (id uuid primary key default uuidv7(), name text);
-- `uuid_extract_version` is NULL for a UUID of a variant other than RFC 9562's, which a CHECK lets
-- through, so only a CHECK beside one that holds it non-null keeps every value to the version.
create table loose (id uuid check (uuid_extract_version(id) = 7));
create table versioned (
  id uuid not null check (uuid_extract_version(id) = 7),
  check (uuid_extract_version(id) is not null)
);
create domain uuid4 as uuid check (uuid_extract_version(value) = 4 and uuid_extract_version(value) is not null);
create table typed (id uuid4);
