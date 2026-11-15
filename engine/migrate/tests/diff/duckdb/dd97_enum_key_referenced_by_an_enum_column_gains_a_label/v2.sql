create type code as enum ('a', 'b', 'z'); create table p (c code primary key, n integer); create table k (id integer, c code references p (c));
