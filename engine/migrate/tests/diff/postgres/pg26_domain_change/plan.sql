alter domain code set default 'new';
alter domain code set not null;
alter domain code drop constraint code_check;
alter domain code add constraint code_check check (value in ('abc', 'xyz', 'new'));
