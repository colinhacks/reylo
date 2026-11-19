create table p (a int, b text, c text) partition by list (a);
create table ch partition of p for values in (1) partition by list (b);
create table g partition of ch for values in ('a');
