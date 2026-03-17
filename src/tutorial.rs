pub struct TutorialLesson {
    pub title: &'static str,
    pub summary: &'static str,
    pub markdown: &'static str,
    pub example: &'static str,
}

pub const LESSONS: &[TutorialLesson] = &[
    TutorialLesson {
        title: "Pitch, Duration, and Relative Mode",
        summary: "Start with note names, durations, and LilyPond's compact `\\relative` pitch entry.",
        markdown: r#"
# Pitch, Duration, and Relative Mode

LilyPond note entry is built from three small ideas:

- Pitch names use letters like `c d e f g a b`
- Durations use numbers like `4` for quarter notes and `8` for eighth notes
- `\relative` avoids repeating octave marks on every note

```lilypond
\relative c' {
  c4 d e f
  g2 g
}
```

## Useful syntax

- `c4` means a quarter-note C
- `g2` means a half-note G
- `c'` raises the octave
- `c,` lowers the octave

## Try this

1. Change the time signature from `4/4` to `3/4`
2. Add a second phrase using `a8 b c d`
3. Replace `\relative` with absolute pitches and compare the difference
"#,
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
        summary: "Use chord brackets and simultaneous voices to build harmony and texture.",
        markdown: r#"
# Chords and Voices

LilyPond supports two common ways to stack notes:

- Chords with angle brackets like `<c e g>`
- Independent voices with `<< ... \\ ... >>`

```lilypond
\relative c' {
  <c e g>1
  << { g'4 f e d } \\ { c2 b } >>
}
```

## Useful syntax

- `<c e g>` plays several notes at once
- `<< ... \\ ... >>` creates upper and lower voices
- `\voiceOne` and `\voiceTwo` improve stem direction on dense passages

## Try this

1. Turn a melody note into a triad
2. Add a bass voice that moves in half notes
3. Use rests in one voice while the other continues
"#,
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
        summary: "Attach text to melody with `\\addlyrics` and shape form with repeat bars.",
        markdown: r#"
# Lyrics and Repeats

Lyrics are attached syllable by syllable to a melody line.

```lilypond
\relative c' {
  c4 d e f
}
\addlyrics {
  Hel -- lo there
}
```

## Useful syntax

- `\addlyrics { ... }` attaches lyrics to the previous voice
- `--` splits syllables across multiple notes
- `__` extends a syllable
- `\repeat volta 2 { ... }` repeats a section with repeat barlines

## Try this

1. Add a pickup note and lyric syllable
2. Repeat the opening phrase with `\repeat volta`
3. Stretch the last lyric syllable with `__`
"#,
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
        summary: "Add phrasing, accents, crescendos, and expressive marks directly in the note stream.",
        markdown: r#"
# Dynamics and Articulation

Musical expression lives inline with the notes.

```lilypond
\relative c' {
  c4\p( d e-> f)
  g2\< a\!
}
```

## Useful syntax

- `\p`, `\mf`, `\f` place dynamics
- `->`, `-.`, and `-^` add articulation
- `(` and `)` create slurs
- `\<` and `\!` start and stop a crescendo hairpin

## Try this

1. Put a crescendo over one measure
2. Accent the highest note
3. Add a slur over a four-note figure
"#,
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
        summary: "Control titles, spacing, margins, and page appearance with LilyPond blocks.",
        markdown: r#"
# Headers, Layout, and Paper

LilyPond keeps score metadata and engraving options in dedicated blocks.

```lilypond
\header {
  title = "Miniature"
  composer = "You"
}

\paper {
  top-margin = 12\mm
}
```

## Useful syntax

- `\header` stores title, composer, subtitle, and more
- `\layout` changes engraving behavior
- `\paper` changes the page itself
- `\markup` adds formatted text outside the staff

## Try this

1. Add a composer name
2. Change the page margins
3. Add a tempo marking with `\tempo`
"#,
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
