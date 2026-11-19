alter table x0 rename constraint x0_id_not_null to "t x_id_not_null";
alter table "t x" attach partition x0 for values from (1) to (4);
