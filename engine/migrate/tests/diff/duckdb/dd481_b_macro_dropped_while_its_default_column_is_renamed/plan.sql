-- b.p2.d is renamed e, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table b.p2 rename column d to e;
alter table b.p2 alter column e drop default;
drop macro b.valid;
alter table b.p2 alter column e set default "valid"();
