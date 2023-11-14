//! Test cases for `links-domainmap`'s unit tests

use crate::ParseError;

/// Sample inputs and expected outputs for [`Domain::reference`]
pub const DOMAIN_REFERENCE: &[(&str, Result<&str, ParseError>)] = &[
	// Valid examples
	("example.com", Ok("example.com")),
	("example.com.", Ok("example.com")),
	("foo.example.com", Ok("foo.example.com")),
	("EXAMPLE.com.", Ok("example.com")),
	("foo.eXaMpLe.com", Ok("foo.example.com")),
	("xnexample.com", Ok("xnexample.com")),
	("xn-example.com", Ok("xn-example.com")),
	// Single-label domains
	("foo", Ok("foo")),
	("a", Ok("a")),
	// Digits
	("a1", Ok("a1")),
	("1", Ok("1")),
	("123.example.com", Ok("123.example.com")),
	("_123.example.com", Ok("_123.example.com")),
	("80.240.24.69", Ok("80.240.24.69")),
	// Empty domain name
	("", Err(ParseError::Empty)),
	(".", Err(ParseError::Empty)),
	// Wildcard in reference identifier
	("*.example.com", Err(ParseError::InvalidChar('*'))),
	// Consecutive dots
	("..", Err(ParseError::LabelEmpty)),
	("..example.com", Err(ParseError::LabelEmpty)),
	("foo..example.com", Err(ParseError::LabelEmpty)),
	("example.com..", Err(ParseError::LabelEmpty)),
	// Dot at the start
	(".foo.example.com", Err(ParseError::LabelEmpty)),
	// Hyphen placement
	("ex-ample.com", Ok("ex-ample.com")),
	("-", Err(ParseError::InvalidHyphen)),
	("-.example", Err(ParseError::InvalidHyphen)),
	("-example.com", Err(ParseError::InvalidHyphen)),
	("example-.com", Err(ParseError::InvalidHyphen)),
	("-ex-ample.com", Err(ParseError::InvalidHyphen)),
	("ex-ample-.com", Err(ParseError::InvalidHyphen)),
	("-ex-ample-.com", Err(ParseError::InvalidHyphen)),
	// Underscores
	("_", Ok("_")),
	("_.com", Ok("_.com")),
	("ex_ample.com", Ok("ex_ample.com")),
	("_example.com", Ok("_example.com")),
	("example_.com", Ok("example_.com")),
	// Invalid ASCII characters
	("ex@mple.com", Err(ParseError::InvalidChar('@'))),
	("example\0.com", Err(ParseError::InvalidChar('\0'))),
	("ex+ample.com", Err(ParseError::InvalidChar('+'))),
	("ex ample.com", Err(ParseError::InvalidChar(' '))),
	(" example.com", Err(ParseError::InvalidChar(' '))),
	// Non-ASCII characters
	("еxample.com", Err(ParseError::InvalidChar('\u{0435}'))),
	("παράδειγμα.com", Err(ParseError::InvalidChar('π'))),
	("xn--hxajbheg2az3al.com", Ok("xn--hxajbheg2az3al.com")),
	// Fake A-label, not checked here
	("xn--example.com", Ok("xn--example.com")),
	// Percent-encoded domain name
	("ex%20ample.com", Err(ParseError::InvalidChar('%'))),
	("e%78ample.com", Err(ParseError::InvalidChar('%'))),
	// 63-character label
	(
		"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com",
		Ok("abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com"),
	),
	// 64-character label
	(
		"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl.com",
		Err(ParseError::LabelTooLong),
	),
	// 253 characters
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
		Ok(
			"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.\
			 q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.\
			 g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
		),
	),
	// 253 characters + trailing dot
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.",
		Ok(
			"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.\
			 q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.\
			 g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
		),
	),
	// 254 characters
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.ww",
		Err(ParseError::TooLong),
	),
	// 254 characters + trailing dot
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.ww.",
		Err(ParseError::TooLong),
	),
];

/// Sample inputs and expected outputs for [`Domain::presented`]
pub const DOMAIN_PRESENTED: &[(&str, Result<&str, ParseError>)] = &[
	// Valid examples
	("example.com", Ok("example.com")),
	("example.com.", Ok("example.com")),
	("foo.example.com", Ok("foo.example.com")),
	("EXAMPLE.com.", Ok("example.com")),
	("foo.eXaMpLe.com", Ok("foo.example.com")),
	("xnexample.com", Ok("xnexample.com")),
	("xn-example.com", Ok("xn-example.com")),
	// Single-label domains
	("foo", Ok("foo")),
	("a", Ok("a")),
	// Digits
	("a1", Ok("a1")),
	("1", Ok("1")),
	("123.example.com", Ok("123.example.com")),
	("_123.example.com", Ok("_123.example.com")),
	("80.240.24.69", Ok("80.240.24.69")),
	// Empty domain name
	("", Err(ParseError::Empty)),
	(".", Err(ParseError::Empty)),
	("*", Err(ParseError::Empty)),
	// Wildcard
	("*.com", Ok("*.com")),
	("*.co.uk", Ok("*.co.uk")),
	("*.example.com", Ok("*.example.com")),
	("f*o.example.com", Err(ParseError::InvalidChar('*'))),
	("fo*.example.com", Err(ParseError::InvalidChar('*'))),
	("*oo.example.com", Err(ParseError::InvalidChar('*'))),
	("foo.*.example.com", Err(ParseError::InvalidChar('*'))),
	// Consecutive dots
	("..", Err(ParseError::LabelEmpty)),
	("..example.com", Err(ParseError::LabelEmpty)),
	("foo..example.com", Err(ParseError::LabelEmpty)),
	("example.com..", Err(ParseError::LabelEmpty)),
	// Dot at the start
	(".foo.example.com", Err(ParseError::LabelEmpty)),
	// Hyphen placement
	("ex-ample.com", Ok("ex-ample.com")),
	("-", Err(ParseError::InvalidHyphen)),
	("-.example", Err(ParseError::InvalidHyphen)),
	("-example.com", Err(ParseError::InvalidHyphen)),
	("example-.com", Err(ParseError::InvalidHyphen)),
	("-ex-ample.com", Err(ParseError::InvalidHyphen)),
	("ex-ample-.com", Err(ParseError::InvalidHyphen)),
	("-ex-ample-.com", Err(ParseError::InvalidHyphen)),
	// Underscores
	("_", Ok("_")),
	("_.com", Ok("_.com")),
	("ex_ample.com", Ok("ex_ample.com")),
	("_example.com", Ok("_example.com")),
	("example_.com", Ok("example_.com")),
	// Invalid ASCII characters
	("ex@mple.com", Err(ParseError::InvalidChar('@'))),
	("example\0.com", Err(ParseError::InvalidChar('\0'))),
	("ex+ample.com", Err(ParseError::InvalidChar('+'))),
	("ex ample.com", Err(ParseError::InvalidChar(' '))),
	(" example.com", Err(ParseError::InvalidChar(' '))),
	// Non-ASCII characters
	("еxample.com", Ok("xn--xample-2of.com")),
	("παράδειγμα.com", Ok("xn--hxajbheg2az3al.com")),
	("xn--hxajbheg2az3al.com", Ok("xn--hxajbheg2az3al.com")),
	// Percent-encoded domain name
	("ex%20ample.com", Err(ParseError::InvalidChar('%'))),
	("e%78ample.com", Err(ParseError::InvalidChar('%'))),
	// 63-character label
	(
		"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com",
		Ok("abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com"),
	),
	// 64-character label
	(
		"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl.com",
		Err(ParseError::LabelTooLong),
	),
	// 63-character label
	(
		"*.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com",
		Ok("*.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com"),
	),
	// 64-character label
	(
		"*.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl.com",
		Err(ParseError::LabelTooLong),
	),
	// 253 characters
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
		Ok(
			"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.\
			 q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.\
			 g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
		),
	),
	// 253 characters + trailing dot
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.",
		Ok(
			"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.\
			 q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.\
			 g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
		),
	),
	// 254 characters
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.ww",
		Err(ParseError::TooLong),
	),
	// 254 characters + trailing dot
	(
		"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.\
		 s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.\
		 k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.ww.",
		Err(ParseError::TooLong),
	),
	// 253 characters
	(
		"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.\
		 r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.\
		 j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v",
		Ok(
			"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.\
			 p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.\
			 f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v",
		),
	),
	// 253 characters + trailing dot
	(
		"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.\
		 r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.\
		 j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.",
		Ok(
			"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.\
			 p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.\
			 f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v",
		),
	),
	// 254 characters
	(
		"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.\
		 r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.\
		 j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.vv",
		Err(ParseError::TooLong),
	),
	// 254 characters + trailing dot
	(
		"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.\
		 r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.\
		 j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.vv.",
		Err(ParseError::TooLong),
	),
];

/// Example input reference identifiers and presented identifiers, and the
/// expected outputs of [`Domain::matches`] and `Domain`'s `PartialEq`
pub const DOMAIN_MATCHES_EQ: &[(&str, &str, bool, bool)] = &[
	// Case sensitivity
	("example.com", "example.com", true, true),
	("example.com", "EXAMPLE.com", true, true),
	("EXAMPLE.com", "example.com", true, true),
	("EXAMPLE.com", "EXAMPLE.com", true, true),
	// Different TLDs
	("example.com", "example.org", false, false),
	("www.foo.example.com", "www.foo.example.org", false, false),
	// Wildcard examples from RFC 6125
	("foo.example.com", "*.example.com", true, false),
	("bar.foo.example.com", "*.example.com", false, false),
	("example.com", "*.example.com", false, false),
	// Absolute domain names
	("example.com.", "example.com", true, true),
	("example.com", "example.com.", true, true),
	("example.com.", "example.com.", true, true),
	("foo.example.com.", "*.example.com", true, false),
	("foo.example.com", "*.example.com.", true, false),
	("foo.example.com.", "*.example.com.", true, false),
	// Internationalized domain names
	(
		"xn--przykad-rjb.xn--fsqu00a.xn--hxajbheg2az3al.com",
		"przykład.例子.παράδειγμα.com",
		true,
		true,
	),
	(
		"xn--przykad-rjb.xn--fsqu00a.xn--hxajbheg2az3al.com",
		"*.例子.παράδειγμα.com",
		true,
		false,
	),
	// Wildcards on public suffixes, allowed by this crate for simplicity
	("example.com", "*.com", true, false),
	("foo.example.com", "*.com", false, false),
	("example.co.uk", "*.co.uk", true, false),
	("foo.example.co.uk", "*.co.uk", false, false),
	("co.uk", "*.co.uk", false, false),
	("example.pvt.k12.ma.us", "*.pvt.k12.ma.us", true, false),
	("pvt.k12.ma.us", "*.pvt.k12.ma.us", false, false),
];

/// Extra tests not in `DOMAIN_MATCHES_EQ` for `presented == presented`
pub const DOMAIN_PRESENTED_EQ_PRESENTED: &[(&str, &str, bool)] = &[
	("*.example.com", "*.example.com", true),
	("*.com", "*.com", true),
	("example.com", "example.com", true),
	("example.com", "example.org", false),
	("*.example.com", "*.example.org", false),
	("*.example.com", "foo.example.com", false),
];

/// Extra tests not in `DOMAIN_MATCHES_EQ` for `reference == reference`
pub const DOMAIN_REFERENCE_EQ_REFERENCE: &[(&str, &str, bool)] = &[
	("example.com", "example.com", true),
	("example.com", "foo.example.com", false),
	("example.com", "example.org", false),
];

/// Extra tests not in `DOMAIN_MATCHES_EQ` for `presented.matches(presented)`
pub const DOMAIN_PRESENTED_MATCHES_PRESENTED: &[(&str, &str, Option<bool>)] = &[
	("*.example.com", "*.example.com", None),
	("*.example.com", "foo.example.com", None),
	("foo.example.com", "foo.example.com", Some(true)),
	("foo.example.com", "bar.example.com", Some(false)),
];

/// Extra tests not in `DOMAIN_MATCHES_EQ` for `reference.matches(reference)`
pub const DOMAIN_REFERENCE_MATCHES_REFERENCE: &[(&str, &str, Option<bool>)] = &[
	("example.com", "example.com", Some(true)),
	("foo.bar.example.com", "foo.bar.example.com", Some(true)),
	("foo.bar.example.com", "example.com", Some(false)),
];

/// Tests for `Domain`'s `Display` implementation (as a tuple of `(input,
/// to_string() output, "{}" formatting output, "{:#}" formatting output)`)
pub const DOMAIN_DISPLAY: &[(&str, &str, &str, &str)] = &[
	(
		"foo.example.com",
		"foo.example.com",
		"foo.example.com",
		"foo.example.com",
	),
	(
		"*.例子.παράδειγμα.com",
		"*.xn--fsqu00a.xn--hxajbheg2az3al.com",
		"*.xn--fsqu00a.xn--hxajbheg2az3al.com",
		"*.例子.παράδειγμα.com",
	),
	(
		"xn--fake-a-label.tld",
		"xn--fake-a-label.tld",
		"xn--fake-a-label.tld",
		"xn--fake-a-label.tld",
	),
];
