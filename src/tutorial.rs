// 定义一个公开的结构体，表示教程中的一节课
pub struct TutorialLesson {
    pub title: &'static str,      // 课程标题，使用静态字符串引用
    pub example: &'static str,    // LilyPond 示例代码，使用静态字符串引用
}

// 定义一个公开的常量切片，包含所有课程
pub const LESSONS: &[TutorialLesson] = &[
    // 第一课：音高、时值和相对模式
    TutorialLesson {
        title: "Pitch, Duration, and Relative Mode",  // 课程标题
        example: r#"\version "2.24.0"  // 原始字符串字面量，包含 LilyPond 代码
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
    // 第二课：和弦和声部
    TutorialLesson {
        // 课程标题
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
    // 第三课：歌词和重复
    TutorialLesson {
        // 课程标题
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
    // 第四课：力度和发音法
    TutorialLesson {
        // 课程标题
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
    // 第五课：页眉、布局和页面设置
    TutorialLesson {
        // 课程标题
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

// 返回默认的 LilyPond 源代码（第一课的示例代码）
pub fn default_source() -> &'static str {
    // 返回第一课的示例代码
    LESSONS[0].example
}
