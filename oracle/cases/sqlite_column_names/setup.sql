-- SQLite names a column with no alias after the text of its expression, so a parameter in that text names it as the statement it runs writes the parameter. The oracle runs each statement as the run time sends it and holds its names to the analysis's.
create table t (a integer not null, b text not null);
insert into t values (1, 'x');
