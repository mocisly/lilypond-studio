pub struct TutorialLesson {
    pub title: &'static str,
    pub example: &'static str,
}

pub const LESSONS: &[TutorialLesson] = &[
    TutorialLesson {
        title: "Pitch, Duration, and Relative Mode",
        example: r#"\version "2.24.0"

\header {
  title = "First Steps"
  subtitle = "Pitch and Duration"
}

\score {
  \new Staff \relative c' {
    \key c \major
    \time 4/4

    c4 d e f
    g2 g
    a4 g f e
    d1
  }

  \layout { }
}"#,
    },
    TutorialLesson {
        title: "Chords and Voices",
        example: r#"\version "2.24.0"

\header {
  title = "Layered Motion"
  subtitle = "Chords and Voices"
}

\score {
  \new Staff <<
    \new Voice = "upper" {
      \voiceOne
      \relative c'' {
        <g b d>2 <a c e>
        g4 f e d
      }
    }
    \new Voice = "lower" {
      \voiceTwo
      \relative c' {
        c2 a
        g2 g
      }
    }
  >>

  \layout { }
}"#,
    },
    TutorialLesson {
        title: "Lyrics and Repeats",
        example: r#"\version "2.24.0"

melody = \relative c' {
  \key g \major
  \time 2/4
  \repeat volta 2 {
    g4 a
    b8 c b a
    g2
  }
}

text = \lyricmode {
  Sing a lit -- tle line,
  hold it now.
}

\score {
  <<
    \new Staff {
      \new Voice = "mel" { \melody }
    }
    \new Lyrics \lyricsto "mel" { \text }
  >>

  \layout { }
}"#,
    },
    TutorialLesson {
        title: "Dynamics and Articulation",
        example: r#"\version "2.24.0"

\score {
  \new Staff \relative c'' {
    \key d \minor
    \time 3/4

    a4\p( bes c)
    d4-> c bes
    a2.\<
    g4\! f\mf e-.
    d2.\f
  }

  \layout { }
}"#,
    },
    TutorialLesson {
        title: "Headers, Layout, and Paper",
        example: r#"\version "2.24.0"

\header {
  title = "Studio Layout Demo"
  composer = "LilyPond Studio"
  tagline = ##f
}

\paper {
  top-margin = 12\mm
  bottom-margin = 12\mm
  left-margin = 15\mm
  right-margin = 15\mm
}

\score {
  \new Staff \relative c' {
    \key f \major
    \time 4/4
    \tempo 4 = 88

    f4 g a bes
    c2 bes
    a4 g f e
    f1
  }

  \layout {
    indent = 0
  }
}"#,
    },
];

pub fn default_source() -> &'static str {
    LESSONS[0].example
}
