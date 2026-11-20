-- loses data: cat.mood
alter table cat
  drop column mood,
  add column mood_id int not null;
create table cat_mood (
  id int not null,
  description varchar(191) not null,
  bite_risk int not null,
  primary key (id)
);
alter table cat add constraint cat_mood_id_fkey foreign key (mood_id) references cat_mood (id) on delete restrict on update cascade;
