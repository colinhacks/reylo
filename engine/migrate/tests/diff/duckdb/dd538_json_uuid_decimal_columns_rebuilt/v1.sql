create table ev (id uuid primary key default gen_random_uuid(), data json, amount decimal(10, 2), happened timestamptz default now(), tags integer[]);
