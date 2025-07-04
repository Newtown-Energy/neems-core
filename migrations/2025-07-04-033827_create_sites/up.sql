-- Your SQL goes here
CREATE TABLE `clients`(
	`id` INT4 NOT NULL PRIMARY KEY,
	`name` VARCHAR NOT NULL
);

CREATE TABLE `sites`(
	`id` INT4 NOT NULL PRIMARY KEY,
	`name` VARCHAR NOT NULL,
	`address` VARCHAR NOT NULL,
	`latitude` FLOAT8 NOT NULL,
	`longitude` FLOAT8 NOT NULL
);

