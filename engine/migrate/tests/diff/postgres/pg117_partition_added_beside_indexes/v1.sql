create table events (event_id integer not null, created_at timestamp not null, data jsonb, primary key (event_id, created_at)) partition by range (created_at);
create index idx_events_created on events (created_at);
create table events_2024 partition of events for values from ('2024-01-01') to ('2025-01-01');
