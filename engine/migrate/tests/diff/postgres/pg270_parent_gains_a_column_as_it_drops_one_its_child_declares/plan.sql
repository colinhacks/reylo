-- loses data: p.e
alter table p drop column e;
alter table h alter column e drop not null;
alter table p add column c boolean;
