-- t.Name is renamed name, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table t rename column Name to name;
