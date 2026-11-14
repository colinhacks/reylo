-- loses data: t
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
