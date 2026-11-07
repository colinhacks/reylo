drop procedure note;
create procedure note(in t text) begin insert into log (line) values (concat('note: ', t)); end;
