alter table only p alter column c set default 'z';
alter table t alter column c set default 'z'::text;
alter table t inherit p;
