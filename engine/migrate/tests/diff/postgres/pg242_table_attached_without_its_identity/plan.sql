alter table x alter column id drop identity;
alter table x rename constraint x_id_not_null to p_id_not_null;
alter table p attach partition x for values from (1) to (4);
