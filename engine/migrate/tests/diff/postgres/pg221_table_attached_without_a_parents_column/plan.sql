alter table x rename constraint x_id_not_null to p_id_not_null;
alter table x add column n text;
alter table p attach partition x for values from (101) to (104);
