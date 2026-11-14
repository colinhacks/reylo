-- SQLite reads a decimal integer literal past 2^63 - 1 as a REAL, and 2^63 under a minus as its smallest INTEGER, -2^63. A hexadecimal literal of up to 64 bits is an INTEGER (`0xffffffffffffffff` is -1), and one past 64 bits is an error.
create table t (id integer primary key, b bigint);
