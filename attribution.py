from bs4 import BeautifulSoup
from datetime import date
from json import load
from requests import get
from subprocess import check_output
from sys import argv

OUTPUT_FILE = "ATTRIBUTION.md"

# Special license handling for certain crates by crate name
NAME_TO_LICENSE_TEXT = load(open("attribution.json", "rt"))

# Get license info using `cargo license`
csv = check_output([
	"cargo",
	"license",
	"--all-features",
	"--authors",
	"--do-not-bundle",
	"--tsv"
]).decode("utf-8").splitlines()[1:]

print(f"{'Outputting certificate info into ' + OUTPUT_FILE:60} [ {0:3} / {len(csv):3} ]")

# Output file header
out = f"""# Dependency Attribution (last updated *{date.today()}*)

**While this repository doesn't contain the dependencies' source code, compiled distributions of links may contain compiled/binary forms of the following direct and transitive dependencies:**
"""

with open(OUTPUT_FILE, "wb") as f:
	f.write(out.encode("utf-8"))

# Parse every line of the TSV output
for i, line in enumerate(csv):
	l = line.split("\t")

	name = l[0].strip()
	version = l[1].strip()
	authors = l[2].strip()
	spdx = l[4].strip()
	link = f"https://docs.rs/crate/{name}/{version}"

	if authors != "":
		authors = " by *" + authors + "*"

	license_text = None

	# Get license text by the crate name or from docs.rs source
	if name in NAME_TO_LICENSE_TEXT:
		license_text = NAME_TO_LICENSE_TEXT[name]
	else:
		text = (
			BeautifulSoup(get(f"{link}/source/UNLICENSE").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENSE-MIT").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENCE-MIT").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENSE-APACHE").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENCE-APACHE").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENSE").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENCE").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENSE.txt").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENSE.md").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/LICENSE-MIT.md").text, "html.parser").find(id="source-code") or
			BeautifulSoup(get(f"{link}/source/license-mit").text, "html.parser").find(id="source-code")
		)

		if text is None:
			print("No license file found for ", name, link, spdx)
			exit()
		else:
			license_text = "```txt\n" + text.get_text().strip() + "\n```"

	print(f"{name + '@' + version:60} [ {i + 1:3} / {len(csv):3} ]")

	# Format crate information and license text
	data = f'''
## [`{name} {version}`]({link}){authors}

{license_text}
'''

	if "-v" in argv:
		print(data)

	# Write the info and license to file
	with open(OUTPUT_FILE, "ab") as f:
		f.write(data.encode("utf-8"))
