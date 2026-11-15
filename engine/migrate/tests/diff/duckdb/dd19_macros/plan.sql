drop macro add1;
create macro add1 (x) as ((x + 2));
create macro fresh (a) as (a);
drop macro table firsts;
create macro firsts (n) as table (select * from "range"((n + 1)));
