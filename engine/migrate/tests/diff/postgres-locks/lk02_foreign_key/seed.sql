insert into users select g from generate_series(1, 1000) g;
insert into posts select g, 1 + g % 1000 from generate_series(1, 3000000) g;
