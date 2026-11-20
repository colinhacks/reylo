create table "Box" ("id" integer not null primary key autoincrement, "material" text not null);
create table "Cat" ("id" integer not null primary key autoincrement, "boxId" integer, constraint "Cat_boxId_fkey" foreign key ("boxId") references "Box" ("id") on delete set null on update cascade);
