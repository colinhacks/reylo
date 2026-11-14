create type zpair as struct(a integer); create macro a_lead(s) as s <> ''; create table t (id integer primary key, p zpair, code varchar check (a_lead(code)));
