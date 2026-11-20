-- loses data: p.c
-- loses data: h.c
alter table p drop column c;
alter table h add column n character varying(20) default '7' not null;
