<!--
A software requirements specification after the outline of ISO/IEC/IEEE
29148, as a Kiln export template, added by `kiln preset add srs`. The
outline follows the standard; the wording here is Kiln's. Edit it
freely: it is this project's file now.

Comments like this one are notes for whoever edits the template: they
don't appear in the export. Replace the placeholder sentences with this
product's own text and delete what doesn't apply. Requirements are
sorted into section 3 by tag (function, performance, usability,
interface, data, attribute); the others land in "Other requirements".

The sections that list items need these types in kiln.toml:
requirement, term, constraint and test, and an [[export]] whose
`template` is this file. The wiki's "Presets" page has that
configuration and its options. `kiln check` validates this file once an
export uses it.
-->

<!-- kiln:summary -->

<!-- kiln:toc -->

# Introduction

## Purpose

What this specification is for and who reads it.

## Scope

The product by name, what it will and won't do, and the benefits and
objectives it serves.

## Product Perspective

<!-- How the product fits with other systems: its interfaces to users, hardware, software and communications, and constraints on operation. -->

Describe the product's context and its interfaces.

## Product Functions

A summary of the major functions; section 3 has the requirements.

## User Characteristics

The groups of users, and their experience and expectations.

## Limitations

<!-- Regulations, hardware limits, interfaces to other applications, safety and security considerations. -->

What limits the developers' options.

## Definitions

<!-- kiln:items type="term" style="compact" -->

# References

<!-- Every document this one refers to: title, version, date, source. -->

# Requirements

## Functions

<!-- kiln:items type="requirement" tag="function" -->

## Performance

<!-- kiln:items type="requirement" tag="performance" -->

## Usability

<!-- kiln:items type="requirement" tag="usability" -->

## Interfaces

<!-- kiln:items type="requirement" tag="interface" -->

## Logical Database

<!-- kiln:items type="requirement" tag="data" -->

## Design Constraints

<!-- kiln:items type="constraint" style="row" -->

## Software System Attributes

<!-- Reliability, availability, security, maintainability, portability. -->

<!-- kiln:items type="requirement" tag="attribute" -->

## Other Requirements

<!-- kiln:items type="requirement" -->

# Verification

<!-- How each requirement is shown to be met: the tests, then the traceability from requirements to them. -->

<!-- kiln:items type="test" style="row" -->

## Traceability

<!-- kiln:report traceability -->

# Appendix: Assumptions and Dependencies

What the requirements assume about the environment, and what they depend
on.

# Appendix: Open Findings

<!-- kiln:report findings -->
