alter table "t x" detach partition "t x_p0";
alter table "t x_p0" rename constraint "t x_id_not_null" to "t x_p0_id_not_null";
