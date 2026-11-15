create type kind as enum ('a', 'b', 'c'); create table p (k kind primary key, n integer); create table c (id integer primary key, k kind references p (k));
