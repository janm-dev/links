//! Test data for `links-domainmap`'s benchmarks

/// Sample inputs for [`Domain::reference`]
#[allow(unused)]
pub const DOMAIN_REFERENCE: &[&str] = &[
	"example.com",
	"example.com.",
	"foo.example.com",
	"EXAMPLE.com.",
	"foo.eXaMpLe.com",
	"xnexample.com",
	"xn-example.com",
	"foo",
	"a",
	"a1",
	"1",
	"123.example.com",
	"_123.example.com",
	"80.240.24.69",
	"ex-ample.com",
	"_",
	"_.com",
	"ex_ample.com",
	"_example.com",
	"example_.com",
	"xn--hxajbheg2az3al.com",
	"xn--example.com",
	"xn--przykad-rjb.xn--fsqu00a.xn--hxajbheg2az3al.com",
	"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com",
	"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.\
	 u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.\
	 o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
	"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.\
	 u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.\
	 o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.",
];

/// Sample inputs for [`Domain::presented`]
#[allow(unused)]
pub const DOMAIN_PRESENTED: &[&str] = &[
	"example.com",
	"example.com.",
	"foo.example.com",
	"EXAMPLE.com.",
	"foo.eXaMpLe.com",
	"xnexample.com",
	"xn-example.com",
	"foo",
	"a",
	"a1",
	"1",
	"123.example.com",
	"_123.example.com",
	"80.240.24.69",
	"*.com",
	"*.co.uk",
	"*.example.com",
	"ex-ample.com",
	"_",
	"_.com",
	"ex_ample.com",
	"_example.com",
	"example_.com",
	"еxample.com",
	"παράδειγμα.com",
	"xn--hxajbheg2az3al.com",
	"xn--przykad-rjb.xn--fsqu00a.xn--hxajbheg2az3al.com",
	"przykład.例子.παράδειγμα.com",
	"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com",
	"*.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk.com",
	"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.\
	 u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.\
	 o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w",
	"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.\
	 u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.\
	 o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.",
	"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.\
	 t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.\
	 n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v",
	"*.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.\
	 t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.\
	 n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.",
];

/// Real, popular domain names in no particular order
pub const REAL_DOMAINS: &[&str] = &[
	"google.it",
	"goo.gl",
	"nytimes.com",
	"cornell.edu",
	"a8.net",
	"prnewswire.com",
	"aboutads.info",
	"imdb.com",
	"thedailybeast.com",
	"webnode.page",
	"ca.gov",
	"globo.com",
	"uol.com.br",
	"amazon.es",
	"bp0.blogger.com",
	"w3.org",
	"ameblo.jp",
	"instagram.com",
	"domainmarket.com",
	"bp1.blogger.com",
	"google.co.in",
	"workspace.google.com",
	"amazon.co.jp",
	"amazon.de",
	"theatlantic.com",
	"thefreedictionary.com",
	"ftc.gov",
	"twitter.com",
	"nginx.org",
	"photobucket.com",
	"www.yahoo.com",
	"ipv4.google.com",
	"nationalgeographic.com",
	"cloudflare.com",
	"independent.co.uk",
	"medium.com",
	"m.wikipedia.org",
	"google.es",
	"ikea.com",
	"microsoft.com",
	"prezi.com",
	"zoom.us",
	"wiley.com",
	"google.fr",
	"washingtonpost.com",
	"clickbank.net",
	"m.me",
	"buydomains.com",
	"vistaprint.com",
	"google.co.jp",
	"tabelog.com",
	"mailchimp.com",
	"indiatimes.com",
	"get.google.com",
	"newscientist.com",
	"who.int",
	"gmail.com",
	"alibaba.com",
	"lavanguardia.com",
	"ouest-france.fr",
	"youronlinechoices.com",
	"foursquare.com",
	"hugedomains.com",
	"yelp.com",
	"cnet.com",
	"un.org",
	"usc.edu",
	"oup.com",
	"goodreads.com",
	"www.wix.com",
	"pinterest.com",
	"dailymail.co.uk",
	"ziddu.com",
	"goal.com",
	"mit.edu",
	"clarin.com",
	"msn.com",
	"cambridge.org",
	"nydailynews.com",
	"www.google.com",
	"cbc.ca",
	"rt.com",
	"hatena.ne.jp",
	"ja.wikipedia.org",
	"netvibes.com",
	"amazon.ca",
	"calendar.google.com",
	"springer.com",
	"www.weebly.com",
	"house.gov",
	"bp2.blogger.com",
	"www.wikipedia.org",
	"fr.wikipedia.org",
	"doubleclick.net",
	"wordpress.org",
	"buzzfeed.com",
	"vice.com",
	"plos.org",
	"qq.com",
	"slate.com",
	"cnbc.com",
	"google.ru",
	"safety.google",
	"smh.com.au",
	"ovhcloud.com",
	"foxnews.com",
	"gofundme.com",
	"steamcommunity.com",
	"techcrunch.com",
	"feedproxy.google.com",
	"es.wikipedia.org",
	"vkontakte.ru",
	"storage.googleapis.com",
	"stuff.co.nz",
	"ggpht.com",
	"huffpost.com",
	"adobe.com",
	"en.wikipedia.org",
	"wa.me",
	"thestar.com",
	"paypal.com",
	"quora.com",
	"disqus.com",
	"photos1.blogger.com",
	"www.livejournal.com",
	"hollywoodreporter.com",
	"addthis.com",
	"aol.com",
	"timeweb.ru",
	"arxiv.org",
	"udemy.com",
	"scholastic.com",
	"bing.com",
	"20minutos.es",
	"usatoday.com",
	"focus.de",
	"walmart.com",
	"forms.gle",
	"deezer.com",
	"digg.com",
	"qz.com",
	"nature.com",
	"support.google.com",
	"linkedin.com",
	"cdc.gov",
	"surveymonkey.com",
	"weibo.com",
	"tools.google.com",
	"issuu.com",
	"sciencedirect.com",
	"waze.com",
	"google.de",
	"wired.com",
	"whatsapp.com",
	"kotaku.com",
	"policies.google.com",
	"ted.com",
	"discord.com",
	"bbc.co.uk",
	"yandex.com",
	"picasa.google.com",
	"yadi.sk",
	"freepik.com",
	"pl.wikipedia.org",
	"bestfreecams.club",
	"harvard.edu",
	"nginx.com",
	"youtube.com",
	"reuters.com",
	"usda.gov",
	"jimdofree.com",
	"dell.com",
	"godaddy.com",
	"automattic.com",
	"behance.net",
	"guardian.co.uk",
	"usnews.com",
	"variety.com",
	"psychologytoday.com",
	"books.google.com",
	"bandcamp.com",
	"shopify.com",
	"amebaownd.com",
	"mozilla.org",
	"tripadvisor.com",
	"canva.com",
	"marketingplatform.google.com",
	"rapidshare.com",
	"metro.co.uk",
	"telegra.ph",
	"samsung.com",
	"sakura.ne.jp",
	"feedburner.com",
	"amazon.co.uk",
	"vk.com",
	"themeforest.net",
	"mynavi.jp",
	"reverbnation.com",
	"xinhuanet.com",
	"instructables.com",
	"excite.co.jp",
	"urbandictionary.com",
	"youtu.be",
	"narod.ru",
	"drive.google.com",
	"welt.de",
	"lefigaro.fr",
	"amazon.fr",
	"gnu.org",
	"lenta.ru",
	"ameba.jp",
	"webmd.com",
	"spotify.com",
	"gstatic.com",
	"disney.com",
	"addtoany.com",
	"draft.blogger.com",
	"tinyurl.com",
	"zippyshare.com",
	"twitch.tv",
	"id.wikipedia.org",
	"e-monsite.com",
	"www.over-blog.com",
	"abcnews.go.com",
	"nasa.gov",
	"alicdn.com",
	"sagepub.com",
	"bfmtv.com",
	"aljazeera.com",
	"redbull.com",
	"news.google.com",
	"bit.ly",
	"translate.google.com",
	"ovh.net",
	"pbs.org",
	"elmundo.es",
	"worldbank.org",
	"marketwatch.com",
	"icann.org",
	"impress.co.jp",
	"weforum.org",
	"dribbble.com",
	"apple.com",
	"wisc.edu",
	"skype.com",
	"secureserver.net",
	"bp.blogspot.com",
	"000webhost.com",
	"archive.org",
	"dan.com",
	"outlook.com",
	"naver.com",
	"sputniknews.com",
	"amazon.in",
	"gutenberg.org",
	"cisco.com",
	"privacyshield.gov",
	"sendspace.com",
	"ria.ru",
	"code.google.com",
	"zdf.de",
	"biglobe.ne.jp",
	"pt.wikipedia.org",
	"coursera.org",
	"creativecommons.org",
	"giphy.com",
	"thesun.co.uk",
	"mystrikingly.com",
	"yandex.ru",
	"fifa.com",
	"line.me",
	"whitehouse.gov",
	"dropbox.com",
	"google.ca",
	"gravatar.com",
	"search.yahoo.com",
	"stanford.edu",
	"elsevier.com",
	"berkeley.edu",
	"plus.google.com",
	"newsweek.com",
	"facebook.com",
	"loc.gov",
	"imageshack.us",
	"afternic.com",
	"zendesk.com",
	"fb.me",
	"parallels.com",
	"vimeo.com",
	"dailymotion.com",
	"it.wikipedia.org",
	"ietf.org",
	"sedoparking.com",
	"about.me",
	"sites.google.com",
	"rtve.es",
	"mediafire.com",
	"office.com",
	"adweek.com",
	"opera.com",
	"faz.net",
	"de.wikipedia.org",
	"google.com.br",
	"picasaweb.google.com",
	"lemonde.fr",
	"home.pl",
	"usgs.gov",
	"britannica.com",
	"www.gov.br",
	"alexa.com",
	"huffingtonpost.com",
	"theguardian.com",
	"thehill.com",
	"xing.com",
	"liveinternet.ru",
	"cnil.fr",
	"esa.int",
	"bbc.com",
	"insider.com",
	"sfgate.com",
	"google.nl",
	"nypost.com",
	"theconversation.com",
	"newyorker.com",
	"theverge.com",
	"nfl.com",
	"offset.com",
	"express.co.uk",
	"elpais.com",
	"mega.nz",
	"marca.com",
	"spiegel.de",
	"news.yahoo.com",
	"android.com",
	"amazon.com",
	"cpanel.com",
	"terra.com.br",
	"myaccount.google.com",
	"123rf.com",
	"amazonaws.com",
	"play.google.com",
	"bloomberg.com",
	"enable-javascript.com",
	"pinterest.fr",
	"abril.com.br",
	"google.pl",
	"dw.com",
	"digitaloceanspaces.com",
	"fb.com",
	"wiktionary.org",
	"time.com",
	"abc.net.au",
	"oracle.com",
	"myspace.com",
	"europa.eu",
	"hubspot.com",
	"www.blogger.com",
	"doi.org",
	"aliexpress.com",
	"mdpi.com",
	"netlify.app",
	"researchgate.net",
	"reg.ru",
	"bloglovin.com",
	"planalto.gov.br",
	"greenpeace.org",
	"xbox.com",
	"abc.es",
	"latimes.com",
	"ndtv.com",
	"google.co.uk",
	"www.gov.uk",
	"huawei.com",
	"wp.com",
	"list-manage.com",
	"amazon.it",
	"windows.net",
	"maps.google.com",
	"sports.yahoo.com",
	"rambler.ru",
	"docs.google.com",
	"groups.google.com",
	"ru.wikipedia.org",
	"cbsnews.com",
	"bild.de",
	"zdnet.com",
	"barnesandnoble.com",
	"tiktok.com",
	"telegram.me",
	"akamaihd.net",
	"developers.google.com",
	"netflix.com",
	"engadget.com",
	"taringa.net",
	"search.google.com",
	"ieee.org",
	"wallpapers.com",
	"t.me",
	"deloitte.com",
	"mashable.com",
	"github.com",
	"ssl-images-amazon.com",
	"ytimg.com",
	"rottentomatoes.com",
	"thenai.org",
	"booking.com",
	"4shared.com",
	"npr.org",
	"googleblog.com",
	"answers.com",
	"brandbucket.com",
	"java.com",
	"about.com",
	"finance.yahoo.com",
	"ovh.com",
	"typeform.com",
	"mail.ru",
	"live.com",
	"telegraph.co.uk",
	"ok.ru",
	"economist.com",
	"wikihow.com",
	"sciencedaily.com",
	"dreamstime.com",
	"dynadot.com",
	"francetvinfo.fr",
	"linktr.ee",
	"target.com",
	"ft.com",
	"pexels.com",
	"photos.google.com",
	"thetimes.co.uk",
	"cutt.ly",
	"pornhub.com",
	"rakuten.co.jp",
	"fandom.com",
	"networkadvertising.org",
	"ibm.com",
	"apache.org",
	"files.wordpress.com",
	"adssettings.google.com",
	"shutterstock.com",
	"cnn.com",
	"mirror.co.uk",
	"yahoo.co.jp",
	"ebay.com",
	"gizmodo.com",
	"soundcloud.com",
	"detik.com",
	"tmz.com",
	"discord.gg",
	"mail.google.com",
	"sedo.com",
	"wikia.com",
	"estadao.com.br",
	"nbcnews.com",
	"kickstarter.com",
	"canada.ca",
	"dropcatch.com",
	"ig.com.br",
	"amzn.to",
	"istockphoto.com",
	"unesco.org",
	"nikkei.com",
	"forbes.com",
	"namecheap.com",
	"hindustantimes.com",
	"loopia.com",
	"academia.edu",
	"cointernet.com.co",
	"scribd.com",
	"cpanel.net",
	"gooyaabitemplates.com",
	"www.canalblog.com",
	"googleusercontent.com",
	"playstation.com",
	"hp.com",
	"change.org",
	"eventbrite.com",
	"marriott.com",
	"wsj.com",
	"businessinsider.com",
	"espn.com",
	"ea.com",
	"indiegogo.com",
	"sapo.pt",
	"ubuntu.com",
	"accounts.google.com",
	"tes.com",
	"pixabay.com",
	"t.co",
	"slideshare.net",
	"php.net",
	"wikimedia.org",
	"steampowered.com",
	"noaa.gov",
	"mozilla.com",
	"plesk.com",
	"nih.gov",
	"unsplash.com",
];