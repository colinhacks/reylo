create type a as enum ('x', 'y'); create type b as enum ('x', 'y', 'z'); create table t (id integer primary key, ca a, cb b);
