create table org (id integer primary key, name varchar);
     create table team (id integer primary key, org_id integer references org (id), name varchar);
     create table person (id integer primary key, org_id integer references org (id), email varchar);
     create table project (id integer primary key, org_id integer references org (id), team_id integer references team (id));
     create table task (id integer primary key, project_id integer references project (id), owner integer references person (id), title varchar);
     create index task_title on task (title);
     create table tag (id integer primary key, task_id integer references task (id));
