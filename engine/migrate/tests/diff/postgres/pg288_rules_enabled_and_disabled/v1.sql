create table t1 ();
create rule rule_disable as on insert to t1 do nothing;
create rule rule_enable as on insert to t1 do nothing;
create rule rule_enable_replica as on insert to t1 do nothing;
create rule rule_enable_always as on insert to t1 do nothing;
alter table t1 disable rule rule_enable;
alter table t1 disable rule rule_enable_replica;
alter table t1 disable rule rule_enable_always;
