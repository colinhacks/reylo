create table readings (id integer not null, at date not null, value numeric) partition by range (at);
create table readings_2026 partition of readings for values from ('2026-01-01') to ('2027-01-01');
