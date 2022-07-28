// XXX DEBUG
const FUN_LINES: &'static [&str] = &[
	"initializing memory segment @ 0x000FF000000...",
	"created boot sequence",
	"scanning regions of nonsense... OK",
	"bringing base modules online.... OK",
	"booting system.... 0%",
	"booting system.... 22%",
	"booting system.... 39%",
	"booting system.... 58%",
	"booting system.... 83%",
	"booting system.... 100%",
	"setting system clock... OK (from NTP server)",
	"connecting to base WiFi antenna... OK",
	"leasing DHCP information... OK",
	"florping sixteen gabfloobers... OK (successfully flooped)",
	"system was booted in a mode that will underperform at any task!",
];

pub fn init() {
	// XXX DEBUG
	FUN_LINES
		.iter()
		.cycle()
		.take(500)
		.for_each(|&line| println!("{}", line));
}
