create table audit (
  id integer not null,
  logged_at timestamp without time zone default (now() at time zone 'utc'),
  constraint audit_pkey primary key (id)
);
alter table events add column flagged boolean default (not 1 is distinct from 2);
alter table events add column logged_at timestamp without time zone default (now() at time zone 'utc');
