alter table p alter column c type timestamp without time zone;
alter table x alter column c type timestamp without time zone;
alter table x rename constraint x_id_not_null to p_id_not_null;
alter table p attach partition x for values in (1);
