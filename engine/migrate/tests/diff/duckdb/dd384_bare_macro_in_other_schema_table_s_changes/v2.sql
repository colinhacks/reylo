create schema s; create macro g(x) as x > 0; create macro s.g(x) as x > 200; create table s.t (a integer, check (g(a)));
