create table cases (id integer not null, name text, constraint cases_id_key unique (id));
create table case_notes (id integer primary key, case_id integer constraint case_notes_case_id_fkey references cases (id) on delete cascade);
