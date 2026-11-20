alter domain a add constraint a_ck check (length(value) > 0);
alter table t alter column n type a using n::text::a;
drop type b;
