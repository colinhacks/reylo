alter table readings detach partition readings_2026;
alter table readings attach partition readings_2026 for values from ('2026-01-01') to ('2027-01-01');
