create table ev (id uuid primary key default gen_random_uuid(), data json check (json_valid(data)), amount decimal(12, 2), happened timestamptz default now(), tags integer[]);
