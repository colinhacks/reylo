create table t (id bigint primary key, tags varchar[], meta struct(host varchar, port integer), counts map(varchar, integer), vec float[3], u union(n integer, s varchar));
