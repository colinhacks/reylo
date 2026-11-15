create macro m1(x) as x > -5; create macro m2(x) as m1(x) and x < 100; create table t (a integer, check (m2(a)));
