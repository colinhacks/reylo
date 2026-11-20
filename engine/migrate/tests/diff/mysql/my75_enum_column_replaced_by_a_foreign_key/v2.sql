create table cat_mood (id integer not null, description varchar(191) not null, bite_risk integer not null, primary key (id));
create table cat (id integer not null, mood_id integer not null, primary key (id), constraint cat_mood_id_fkey foreign key (mood_id) references cat_mood (id) on delete restrict on update cascade);
