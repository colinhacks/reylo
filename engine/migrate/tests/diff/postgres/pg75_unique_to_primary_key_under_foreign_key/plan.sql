alter table case_notes drop constraint case_notes_case_id_fkey;
alter table cases drop constraint cases_id_key;
alter table cases add constraint cases_pkey primary key (id);
alter table case_notes add constraint case_notes_case_id_fkey foreign key (case_id) references cases(id) on delete cascade;
