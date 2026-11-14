alter table t alter column s drop default;
drop macro b.tag;
create macro b.tag (s) as (concat(s, '!'));
alter table t alter column s set default b.tag('x');
