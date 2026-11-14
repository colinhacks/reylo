create table org (id integer primary key, name varchar check (name <> ''));
     create table team (id integer primary key, org_id integer references org (id), name varchar, size integer default 0);
     create table person (id integer primary key, org_id integer references org (id), mail varchar);
     create table project (id integer primary key, org_id integer references org (id), team_id integer references team (id));
     create table task (id integer primary key, project_id integer references project (id), owner integer references person (id), title varchar, done boolean default false);
     create index task_title on task (title);
     comment on table task is 'work';
     create table tag (id integer primary key, task_id integer references task (id), label varchar);
