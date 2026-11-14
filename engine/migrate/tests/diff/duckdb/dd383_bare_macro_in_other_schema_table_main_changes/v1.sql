create schema s; create macro g(x) as x > 0; create macro s.g(x) as x > 100; create table s.t (a integer, check (g(a)));
