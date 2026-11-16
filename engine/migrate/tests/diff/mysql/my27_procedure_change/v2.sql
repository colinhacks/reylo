create table log (id int primary key auto_increment, line text); create procedure note(in t text) begin insert into log (line) values (concat('note: ', t)); end;
